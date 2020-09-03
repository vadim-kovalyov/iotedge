// Copyright (c) Microsoft. All rights reserved.
namespace Microsoft.Azure.Devices.Edge.Hub.Core
{
    using System;
    using System.Linq;
    using System.Threading.Tasks;
    using Microsoft.Azure.Devices.Edge.Hub.Core.Device;
    using Microsoft.Azure.Devices.Edge.Hub.Core.Identity;
    using Microsoft.Azure.Devices.Edge.Hub.Core.Identity.Service;
    using Microsoft.Azure.Devices.Edge.Util;
    using Microsoft.Extensions.Logging;

    public abstract class DeviceScopeAuthenticator<T> : IAuthenticator
        where T : IClientCredentials
    {
        readonly IDeviceScopeIdentitiesCache deviceScopeIdentitiesCache;
        readonly IAuthenticator underlyingAuthenticator;
        readonly bool allowDeviceAuthForModule;
        readonly bool syncServiceIdentityOnFailure;
        readonly bool nestedEdgeEnabled;

        protected DeviceScopeAuthenticator(
            IDeviceScopeIdentitiesCache deviceScopeIdentitiesCache,
            IAuthenticator underlyingAuthenticator,
            bool allowDeviceAuthForModule,
            bool syncServiceIdentityOnFailure,
            bool nestedEdgeEnabled)
        {
            this.underlyingAuthenticator = Preconditions.CheckNotNull(underlyingAuthenticator, nameof(underlyingAuthenticator));
            this.deviceScopeIdentitiesCache = Preconditions.CheckNotNull(deviceScopeIdentitiesCache, nameof(deviceScopeIdentitiesCache));
            this.allowDeviceAuthForModule = allowDeviceAuthForModule;
            this.syncServiceIdentityOnFailure = syncServiceIdentityOnFailure;
            this.nestedEdgeEnabled = nestedEdgeEnabled;
        }

        public async Task<bool> AuthenticateAsync(IClientCredentials clientCredentials)
        {
            if (!(clientCredentials is T tCredentials))
            {
                return false;
            }

            (bool isAuthenticated, bool shouldFallback) = await this.AuthenticateInternalAsync(tCredentials, false);
            Events.AuthenticatedInScope(clientCredentials.Identity, isAuthenticated);
            if (!isAuthenticated && shouldFallback)
            {
                isAuthenticated = await this.underlyingAuthenticator.AuthenticateAsync(clientCredentials);
            }

            return isAuthenticated;
        }

        public async Task<bool> ReauthenticateAsync(IClientCredentials clientCredentials)
        {
            if (!(clientCredentials is T tCredentials))
            {
                return false;
            }

            (bool isAuthenticated, bool shouldFallback) = await this.AuthenticateInternalAsync(tCredentials, true);
            Events.ReauthenticatedInScope(clientCredentials.Identity, isAuthenticated);
            if (!isAuthenticated && shouldFallback)
            {
                Events.ServiceIdentityNotFound(tCredentials.Identity);
                isAuthenticated = await this.underlyingAuthenticator.ReauthenticateAsync(clientCredentials);
            }

            return isAuthenticated;
        }

        protected abstract (Option<string> deviceId, Option<string> moduleId) GetActorId(T credentials);

        protected abstract bool AreInputCredentialsValid(T credentials);

        protected abstract bool ValidateWithServiceIdentity(ServiceIdentity serviceIdentity, T credentials, Option<string> authChain);

        async Task<(bool isAuthenticated, bool shouldFallback)> AuthenticateInternalAsync(T tCredentials, bool reauthenticating)
        {
            try
            {
                if (!this.AreInputCredentialsValid(tCredentials))
                {
                    Events.InputCredentialsNotValid(tCredentials.Identity);
                    return (false, false);
                }

                bool syncServiceIdentity = this.syncServiceIdentityOnFailure && !reauthenticating;
                bool isAuthenticated = false;
                bool valueFound = false;
                string actorId = tCredentials.Identity.Id;

                // Try to get the actorId from the credentials
                (Option<string> actorDeviceId, Option<string> actorModuleId) = this.GetActorId(tCredentials);
                if (actorDeviceId.HasValue)
                {
                    actorId = actorDeviceId.OrDefault() + actorModuleId.Map(id => "/" + id).GetOrElse(string.Empty);
                }

                if (this.nestedEdgeEnabled &&
                    actorModuleId.Contains(Constants.EdgeHubModuleId) &&
                    actorId != tCredentials.Identity.Id)
                {
                    // OnBehalfOf means an EdgeHub is trying to use its own auth to
                    // connect as a child device or module. So if acting EdgeHub is
                    // different than the target identity, then we know we're
                    // processing an OnBehalfOf authentication.
                    (isAuthenticated, valueFound) = await this.AuthenticateWithAuthChain(tCredentials, actorId, syncServiceIdentity);
                }
                else
                {
                    // Default scenario where the credential is for the target identity
                    (isAuthenticated, valueFound) = await this.AuthenticateWithServiceIdentity(tCredentials, tCredentials.Identity.Id, syncServiceIdentity, Option.None<string>());

                    if (!isAuthenticated && this.allowDeviceAuthForModule && tCredentials.Identity is IModuleIdentity moduleIdentity)
                    {
                        // The module could have used the Device key to sign its token
                        Events.AuthenticatingWithDeviceIdentity(moduleIdentity);
                        (isAuthenticated, valueFound) = await this.AuthenticateWithServiceIdentity(tCredentials, moduleIdentity.DeviceId, syncServiceIdentity, Option.None<string>());
                    }
                }

                // In the return value, the first flag indicates if the authentication succeeded.
                // The second flag indicates whether the authenticator should fall back to the underlying authenticator. This is done if
                // the ServiceIdentity was not found (which means the device/module is not in scope).
                return (isAuthenticated, !valueFound);
            }
            catch (Exception e)
            {
                Events.ErrorAuthenticating(e, tCredentials, reauthenticating);
                return (false, true);
            }
        }

        async Task<(bool isAuthenticated, bool serviceIdentityFound)> AuthenticateWithAuthChain(T credentials, string actorEdgeHubId, bool syncServiceIdentity)
        {
            // For nested Edge, we need to check that we have
            // a valid authchain for the target identity
            Option<string> authChain = await this.deviceScopeIdentitiesCache.GetAuthChain(credentials.Identity.Id);
            if (!authChain.HasValue)
            {
                Events.NoAuthChain(credentials.Identity);
                return (false, false);
            }

            return await this.AuthenticateWithServiceIdentity(credentials, actorEdgeHubId, syncServiceIdentity, authChain);
        }

        async Task<(bool isAuthenticated, bool serviceIdentityFound)> AuthenticateWithServiceIdentity(T credentials, string serviceIdentityId, bool syncServiceIdentity, Option<string> authChain)
        {
            Option<ServiceIdentity> serviceIdentity = await this.deviceScopeIdentitiesCache.GetServiceIdentity(serviceIdentityId);
            (bool isAuthenticated, bool serviceIdentityFound) = serviceIdentity.Map(s => (this.ValidateWithServiceIdentity(s, credentials, authChain), true)).GetOrElse((false, false));

            if (!isAuthenticated && (!serviceIdentityFound || syncServiceIdentity))
            {
                Events.ResyncingServiceIdentity(credentials.Identity, serviceIdentityId);
                await this.deviceScopeIdentitiesCache.RefreshServiceIdentity(serviceIdentityId);
                serviceIdentity = await this.deviceScopeIdentitiesCache.GetServiceIdentity(serviceIdentityId);

                if (!serviceIdentity.HasValue)
                {
                    // Identity still doesn't exist, this can happen if the identity
                    // is newly added and we couldn't refresh the individual identity
                    // because we don't know where it resides in the nested hierarchy.
                    // In this case our only recourse is to refresh the whole cache
                    // and hope the identity shows up.
                    this.deviceScopeIdentitiesCache.InitiateCacheRefresh();
                    await this.deviceScopeIdentitiesCache.WaitForCacheRefresh(TimeSpan.FromSeconds(100));
                    serviceIdentity = await this.deviceScopeIdentitiesCache.GetServiceIdentity(serviceIdentityId);
                }

                (isAuthenticated, serviceIdentityFound) = serviceIdentity.Map(s => (this.ValidateWithServiceIdentity(s, credentials, authChain), true)).GetOrElse((false, false));
            }

            return (isAuthenticated, serviceIdentityFound);
        }

        static class Events
        {
            const int IdStart = HubCoreEventIds.DeviceScopeAuthenticator;
            static readonly ILogger Log = Logger.Factory.CreateLogger<DeviceScopeAuthenticator<T>>();

            enum EventIds
            {
                ErrorAuthenticating = IdStart,
                ServiceIdentityNotFound,
                NoAuthChain,
                AuthenticatedInScope,
                InputCredentialsNotValid,
                ResyncingServiceIdentity,
                AuthenticatingWithDeviceIdentity
            }

            public static void ErrorAuthenticating(Exception exception, IClientCredentials credentials, bool reauthenticating)
            {
                string operation = reauthenticating ? "reauthenticating" : "authenticating";
                Log.LogWarning((int)EventIds.ErrorAuthenticating, exception, $"Error {operation} credentials for {credentials.Identity.Id}");
            }

            public static void ServiceIdentityNotFound(IIdentity identity)
            {
                Log.LogDebug((int)EventIds.ServiceIdentityNotFound, $"Service identity for {identity.Id} not found. Using underlying authenticator to authenticate");
            }

            public static void NoAuthChain(IIdentity identity)
            {
                Log.LogDebug((int)EventIds.NoAuthChain, $"Could not get valid auth-chain for service identity {identity.Id}");
            }

            public static void AuthenticatedInScope(IIdentity identity, bool isAuthenticated)
            {
                string authenticated = isAuthenticated ? "authenticated" : "not authenticated";
                Log.LogInformation((int)EventIds.AuthenticatedInScope, $"Client {identity.Id} in device scope {authenticated} locally.");
            }

            public static void ReauthenticatedInScope(IIdentity identity, bool isAuthenticated)
            {
                string authenticated = isAuthenticated ? "reauthenticated" : "not reauthenticated";
                Log.LogDebug((int)EventIds.AuthenticatedInScope, $"Client {identity.Id} in device scope {authenticated} locally.");
            }

            public static void InputCredentialsNotValid(IIdentity identity)
            {
                Log.LogInformation((int)EventIds.InputCredentialsNotValid, $"Credentials for client {identity.Id} are not valid.");
            }

            public static void ResyncingServiceIdentity(IIdentity identity, string serviceIdentityId)
            {
                Log.LogInformation((int)EventIds.ResyncingServiceIdentity, $"Unable to authenticate client {identity.Id} with cached service identity {serviceIdentityId}. Resyncing service identity...");
            }

            public static void AuthenticatingWithDeviceIdentity(IModuleIdentity moduleIdentity)
            {
                Log.LogInformation((int)EventIds.AuthenticatingWithDeviceIdentity, $"Unable to authenticate client {moduleIdentity.Id} with module credentials. Attempting to authenticate using device {moduleIdentity.DeviceId} credentials.");
            }
        }
    }
}
