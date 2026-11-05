package demo

class OrderPlacedHandler(private val outbox: Outbox) : AuditedHandler<OrderPlaced>() {
    override fun onEvent(event: OrderPlaced) {
        outbox.sent += "placed ${event.id}"
    }
}

class OrderCancelledHandler(private val outbox: Outbox) : EventHandler<OrderCancelled> {
    override fun handle(event: OrderCancelled) {
        outbox.sent += "cancelled ${event.id}"
    }
}
