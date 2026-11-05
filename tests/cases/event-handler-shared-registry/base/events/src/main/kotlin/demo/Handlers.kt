package demo

class OrderPlacedHandler(private val outbox: Outbox) : AuditedHandler<OrderPlaced>() {
    override fun handle(event: OrderPlaced) {
        record(event)
        outbox.sent += "placed ${event.id}"
    }
}

class OrderCancelledHandler(private val outbox: Outbox) : EventHandler<OrderCancelled> {
    override fun handle(event: OrderCancelled) {
        outbox.sent += "cancelled ${event.id}"
    }
}

fun bus(outbox: Outbox): EventBus = EventBus(listOf(OrderPlacedHandler(outbox), OrderCancelledHandler(outbox)))
