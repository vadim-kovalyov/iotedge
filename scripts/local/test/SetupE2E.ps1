#!/bin/pwsh

## Copyright (c) Microsoft. All rights reserved.

Param(
    [string]$imageTag, 
    [string]$packagesPath)

# get secrets here
# - task: AzureKeyVault@1
#   displayName: Get secrets
#   inputs:
#     azureSubscription: $(az.subscription)
#     keyVaultName: $(kv.name)
#     secretsFilter: >-
#       TestContainerRegistryPassword,
#       TestDpsGroupKeySymmetric,
#       TestEventHubCompatibleEndpoint,
#       TestGitHubAccessToken,
#       TestPreviewEventHubCompatibleEndpoint,
#       TestIotedgedPackageRootSigningCert,
#       TestIotHubConnectionString,
#       TestRootCaCertificate,
#       TestRootCaKey,
#       TestRootCaPassword,
#       TestBlobStoreSas

$TestContainerRegistryPassword = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestContainerRegistryPassword" -AsPlainText
$TestDpsGroupKeySymmetric = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestDpsGroupKeySymmetric" -AsPlainText
$TestEventHubCompatibleEndpoint = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestEventHubCompatibleEndpoint" -AsPlainText

$TestPreviewEventHubCompatibleEndpoint = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestPreviewEventHubCompatibleEndpoint" -AsPlainText
$TestIotedgedPackageRootSigningCert = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestIotedgedPackageRootSigningCert" -AsPlainText
$TestIotHubConnectionString = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestIotHubConnectionString" -AsPlainText
$TestRootCaCertificate = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestRootCaCertificate" -AsPlainText
$TestRootCaKey = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestRootCaKey" -AsPlainText
$TestRootCaPassword = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestRootCaPassword" -AsPlainText
$TestBlobStoreSas = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestBlobStoreSas" -AsPlainText

if ($packagesPath) {
    ## Download aziot-identity-service
    $env:GITHUB_TOKEN = Get-AzKeyVaultSecret -VaultName $vaultName -Name "TestGitHubAccessToken" -AsPlainText
    $env:DOWNLOAD_PATH = $packagesPath

    ./scripts/local/test/DownloadIdentityService.ps1
    # GITHUB_TOKEN: $(TestGitHubAccessToken)
    # ARTIFACT_NAME: $(identityServiceArtifactName)
    # PACKAGE_FILTER: $(identityServicePackageFilter)
    # DOWNLOAD_PATH: $packagesPath
    # IDENTITY_SERVICE_COMMIT: $(aziotis.commit)
}

## Install CA keys
$certsDir = '$(System.ArtifactsDirectory)/certs'
New-Item "$certsDir" -ItemType Directory -Force | Out-Null
$env:ROOT_CERT | Out-File -Encoding Utf8 "$certsDir/rsa_root_ca.cert.pem"
$env:ROOT_KEY | Out-File -Encoding Utf8 "$certsDir/rsa_root_ca.key.pem"


## Build tests
$testDir = '$(Build.SourcesDirectory)/test/Microsoft.Azure.Devices.Edge.Test'
dotnet build $testDir

$binDir = Convert-Path "$testDir/bin/Debug/netcoreapp3.1"
# env:
# http_proxy: $(Agent.ProxyUrl)
# https_proxy: $(Agent.ProxyUrl)

#Create test arguments file (context.json)
$imageId = Get-Content -Encoding Utf8 `
    '$(System.ArtifactsDirectory)/$(az.pipeline.images.artifacts)/artifactInfo.txt'
$imageId = ($imageId -split '=')[1]
$imageTag = "$imageId-$(os)-$(arch)"

if ('$(nestededge)' -eq 'true') {
    $nestededge = "true"
    $imagePrefix = '$upstream:443/$(cr.labelPrefix)azureiotedge'
    $diagnosticImagePrefix = '$(parentName):443/$(cr.labelPrefix)azureiotedge'
    $caCertScriptPath = Convert-Path "/certs"
    $rootCaCertificatePath = Convert-Path "/certs/certs/azure-iot-test-only.root.ca.cert.pem"
    $rootCaPrivateKeyPath = Convert-Path "/certs/private/azure-iot-test-only.root.ca.key.pem"
}
else {
    $nestededge = "false"
    $imagePrefix = '$(cr.address)/$(cr.labelPrefix)azureiotedge'
    $diagnosticImagePrefix = '$(cr.address)/$(cr.labelPrefix)azureiotedge'
    $caCertScriptPath = Convert-Path '$(Build.SourcesDirectory)/tools/CACertificates'
    $rootCaCertificatePath = Convert-Path '$(certsDir)/rsa_root_ca.cert.pem';
    $rootCaPrivateKeyPath = Convert-Path '$(certsDir)/rsa_root_ca.key.pem';
}

if ('$(customEdgeAgent.image)') {
    $edgeAgentImage = "$(customEdgeAgent.image)"; 
}
else {
    $edgeAgentImage = "$imagePrefix-agent:$imageTag";
}

if ('$(customEdgeHub.image)') {
    $edgeHubImage = "$(customEdgeHub.image)";
}
else {
    $edgeHubImage = "$imagePrefix-hub:$imageTag";
}

echo "Edge agent image: $edgeAgentImage"
echo "Edge hub image: $edgeHubImage"

$context = @{
    nestededge                 = "$nestededge";
    dpsIdScope                 = '$(dps.idScope)';
    isa95Tag                   = 'false';
    edgeAgentImage             = "$edgeAgentImage";
    edgeHubImage               = "$edgeHubImage";
    diagnosticsImage           = "$diagnosticImagePrefix-diagnostics:$imageTag";
    tempFilterFuncImage        = "$imagePrefix-functions-filter:$imageTag";
    tempFilterImage            = "$imagePrefix-temperature-filter:$imageTag";
    tempSensorImage            = "$imagePrefix-simulated-temperature-sensor:$imageTag";
    methodSenderImage          = "$imagePrefix-direct-method-sender:$imageTag";
    methodReceiverImage        = "$imagePrefix-direct-method-receiver:$imageTag";
    loadGenImage               = "$imagePrefix-load-gen:$imageTag";
    relayerImage               = "$imagePrefix-relayer:$imageTag";
    networkControllerImage     = "$imagePrefix-network-controller:$imageTag";
    testResultCoordinatorImage = "$imagePrefix-test-result-coordinator:$imageTag";
    metricsValidatorImage      = "$imagePrefix-metrics-validator:$imageTag";
    numberLoggerImage          = "$imagePrefix-number-logger:$imageTag";
    edgeAgentBootstrapImage    = "$imagePrefix-agent-bootstrap-e2e-$(os)-$(arch)";
    registries                 = @(
        @{
            address  = '$(cr.address)';
            username = '$(cr.username)';
        }
    );
    packagePath                = Convert-Path '$(System.ArtifactsDirectory)/$(artifactName)';
    caCertScriptPath           = "$caCertScriptPath";
    rootCaCertificatePath      = "$rootCaCertificatePath";
    rootCaPrivateKeyPath       = "$rootCaPrivateKeyPath";
    logFile                    = Join-Path '$(binDir)' 'testoutput.log';
}

if ('$(nestededge)' -eq 'true') {
    $context['hostname'] = '$(deviceName)'
    $context['parentHostname'] = '$(parentName)'
    $context['parentDeviceId'] = '$(parentDeviceId)'

    if ('$(test_type)' -eq 'nestededge_isa95') {
        $context['deviceId'] = '$(Lvl3DeviceId)'
        $context['edgeProxy'] = '$(proxyAddress)'
        $context['isa95Tag'] = 'true'
    }
}

if ('$(arch)' -eq 'arm32v7' -Or '$(arch)' -eq 'arm64v8') {
    $context['optimizeForPerformance'] = 'false'
    $context['setupTimeoutMinutes'] = 10
    $context['teardownTimeoutMinutes'] = 10
    $context['testTimeoutMinutes'] = 10
}

if ($env:AGENT_PROXYURL) {
    $context['testRunnerProxy'] = $env:AGENT_PROXYURL
    $context['edgeProxy'] = $env:AGENT_PROXYURL
}

$context | ConvertTo-Json | Out-File -Encoding Utf8 '$(binDir)/context.json'
  