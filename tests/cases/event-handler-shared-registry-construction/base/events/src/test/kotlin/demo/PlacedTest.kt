package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class PlacedTest {
    @Test
    fun placingSendsMail() {
        val outbox = Outbox()
        bus(outbox).publish(OrderPlaced(1))
        assertEquals(listOf("placed 1"), outbox.sent)
    }
}
