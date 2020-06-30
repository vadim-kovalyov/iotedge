package edgehub

default allow = false

allow {
	allow_operation
}

allow_operation {
	operation_is_connect
}

allow_operation {
	operation_is_publish
}

allow_operation {
	operation_is_subscribe
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
