package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class CancelledTest {
    @Test
    fun cancellingSendsMail() {
        val outbox = Outbox()
        bus(outbox).publish(OrderCancelled(1))
        assertEquals(listOf("cancelled 1"), outbox.sent)
    }
}
