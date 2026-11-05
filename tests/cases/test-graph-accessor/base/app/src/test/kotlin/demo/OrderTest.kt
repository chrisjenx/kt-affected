package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class OrderTest {
    private val ext = ServicesExtension()

    @Test
    fun places() {
        assertEquals("order:hay", ext.services.orders.place("hay"))
    }
}
