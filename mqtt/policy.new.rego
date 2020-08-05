package edgehub

default allow = false

allow {
    iothub_policy
    custom_policy
}

# allow only non-anonymous clients whose auth_id and client_id identical
iothub_policy {
    operation_is_connect
    not anonymous_client
    client_id_matches_auth_id
}

operation_is_connect {
    input.operation.type == "Connect"
}

anonymous_client {
    input.client_info.auth_id.Identity == ""
}

client_id_matches_auth_id {
    input.client_id == input.client_info.auth_id.Identity
}

# allow any client to publish to any non-iothub topics
iothub_policy {
    operation_is_publish
    not it_is_iothub_topic
    not topic_is_forbidden
}

# allow client to publish to it's iothub topics.
iothub_policy {
    operation_is_publish
    it_is_iothub_topic
    it_is_allowed_iothub_topic
}

operation_is_publish {
    input.operation.type == "Publish"
}

it_is_iothub_topic {
    iothub_topic_prefix := ["$iothub/", "$edgehub/"]
    startswith(input.operation.publication.topic_name, iothub_topic_prefix[_])
}

topic_is_forbidden {
    topic_forbidden_prefix := ["$", "#"]
    not it_is_iothub_topic_filter    
    startswith(input.operation.publication.topic_name, topic_forbidden_prefix[_])
}

it_is_allowed_iothub_topic {
    iothub_topic_patterns := [
        "$edgehub/clients/{}/messages/events",
        "$iothub/clients/{}/messages/events",
        "$edgehub/clients/{}/twin/get",
        "$iothub/clients/{}/twin/get"
    ]
    replace(iothub_topic_patterns[pattern], "{}", input.client_id) == input.operation.publication.topic_name
}

# allow any client to subscribe to any non-iothub topics
iothub_policy {
    operation_is_subscribe
    not it_is_iothub_topic_filter
    not topic_filter_is_forbidden
}

# allow client to subscribe to it's iothub topics.
iothub_policy {
    operation_is_subscribe
    it_is_iothub_topic_filter 
    it_is_allowed_iothub_topic_filter 
}

operation_is_subscribe {
    input.operation.type == "Subscribe"
}

topic_filter_is_forbidden {
    topic_forbidden_prefix := ["$", "#"]
    not it_is_iothub_topic_filter    
    startswith(input.operation.topic_filter, topic_forbidden_prefix[_])
}

it_is_iothub_topic_filter  {
    iothub_topic_prefix := ["$iothub/", "$edgehub/"]
    startswith(input.operation.topic_filter, iothub_topic_prefix[_])
}

it_is_allowed_iothub_topic_filter {
    iothub_topic_patterns := [
        "$edgehub/clients/{}/messages/events",
        "$iothub/clients/{}/messages/events",
        "$edgehub/clients/{}/twin/get",
        "$iothub/clients/{}/twin/get"
    ]
    replace(iothub_topic_patterns[pattern], "{}", input.client_id) == input.operation.topic_filter
}

#########################

custom_policy {
    custom_connect_policy
}

custom_policy {
    custom_publish_policy
}

custom_policy {
    custom_subscribe_policy
}

custom_connect_policy {
    connect_allowed
    not connect_denied
}

connect_allowed {
    input.operation.type = "Connect"
    data.allow[record].operation[0] = "connect"
    rr := data.allow[record].identity
    rc := replace(rr, "{{client_id}}", input.client_id)
	ra := replace(rc, "{{auth_id}}", input.client_info.auth_id.Identity)
    ra = input.client_info.auth_id.Identity
}

connect_denied {
    input.operation.type = "Connect"
    data.deny[record].operation[0] = "connect"
    rr := data.deny[record].identity
    rc := replace(rr, "{{client_id}}", input.client_id)
	ra := replace(rc, "{{auth_id}}", input.client_info.auth_id.Identity)
    ra = input.client_info.auth_id.Identity
}

custom_publish_policy {
    publish_allowed
    not publish_denied
}

publish_allowed {
    input.operation.type = "Publish"
    data.allow[record].identity = input.client_info.auth_id.Identity
    data.allow[record].operation = ["publish"]
    rr := data.allow[record].resource[topic]
    rc := replace(rr, "{{client_id}}", input.client_id)
	ra := replace(rc, "{{auth_id}}", input.client_info.auth_id.Identity)
	ra = input.operation.publication.topic_name
}

publish_denied {
    input.operation.type = "Publish"
    data.deny[record].identity = input.client_info.auth_id.Identity
    data.deny[record].operation = ["publish"]
    rr := data.deny[record].resource[topic]
    rc := replace(rr, "{{client_id}}", input.client_id)
	ra := replace(rc, "{{auth_id}}", input.client_info.auth_id.Identity)
	ra = input.operation.publication.topic_name
}

custom_subscribe_policy {
	false
}