package edgehub

default allow = false

allow {
	allow_operation
}

# allow only non-anonymous clients whose auth_id and client_id identical
allow_operation {
	operation_is_connect
    not anonymous_client
    client_id_matches_auth_id
}

# allow any client to publish to any non-iothub topics
allow_operation {
	operation_is_publish
    not it_is_iothub_topic_pub
    not topic_is_forbidden_pub
}

# allow client to publish to it's iothub topics.
allow_operation {
	operation_is_publish
    it_is_iothub_topic_pub
    it_is_allowed_iothub_topic_pub
}

# allow any client to subscribe to any non-iothub topics
allow_operation {
	operation_is_subscribe
    not it_is_iothub_topic_sub
    not topic_is_forbidden_sub
}

# allow client to subscribe to it's iothub topics.
allow_operation {
	operation_is_subscribe
    it_is_iothub_topic_sub
    it_is_allowed_iothub_topic(input.operation.)
}

operation_is_connect {
	input.operation.type == "Connect"
}

operation_is_publish {
	input.operation.type == "Publish"
}

operation_is_subscribe {
	input.operation.type == "Subscribe"
}

anonymous_client {
	input.auth_id == ""
}

client_id_matches_auth_id {
    input.client_id == input.auth_id
}

topic_is_forbidden(topic) = {
	not it_is_iothub_topic_pub
	topic_forbidden_prefix := ["$", "#"]
	startswith(topic, topic_forbidden_prefix[_])
}

it_is_iothub_topic(topic) = {
	iothub_topic_prefix := ["$iothub/", "$edgehub/"]
	startswith(topic, iothub_topic_prefix[_])
}

it_is_allowed_iothub_topic(topic) = {
	iothub_topic_patterns := [
        "$edgehub/clients/{}/messages/events",
        "$iothub/clients/{}/messages/events",
        "$edgehub/clients/{}/twin/get",
        "$iothub/clients/{}/twin/get"
    ],
    replace(iothub_topic_patterns[pattern], "{}", input.client_id) == topic
}
