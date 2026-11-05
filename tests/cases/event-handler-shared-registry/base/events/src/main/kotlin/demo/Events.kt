package demo

interface DomainEvent

data class OrderPlaced(val id: Int) : DomainEvent

data class OrderCancelled(val id: Int) : DomainEvent

interface EventHandler<E : DomainEvent> {
    fun handle(event: E)
}

abstract class AuditedHandler<E : DomainEvent> : EventHandler<E> {
    val audit = mutableListOf<String>()

    fun record(event: E) {
        audit += event.toString()
    }
}

class Outbox {
    val sent = mutableListOf<String>()
}

class EventBus(private val handlers: List<EventHandler<*>>) {
    @Suppress("UNCHECKED_CAST")
    fun publish(event: DomainEvent) {
        // Erased generics: a handler for another event fails its bridge cast and is skipped.
        for (h in handlers) runCatching { (h as EventHandler<DomainEvent>).handle(event) }
    }
}
