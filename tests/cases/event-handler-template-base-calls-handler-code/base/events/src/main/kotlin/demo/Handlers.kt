package demo

class OrderPlacedHandler(private val outbox: Outbox) : AuditedHandler<OrderPlaced>() {
    companion object { fun tag(): String = "v1" }
    override fun onEvent(event: OrderPlaced) {
        outbox.sent += "placed ${event.id}"
    }
}

class OrderCancelledHandler(private val outbox: Outbox) : AuditedHandler<OrderCancelled>() {
    override fun onEvent(event: OrderCancelled) {
        outbox.sent += "cancelled ${event.id}"
    }
}
