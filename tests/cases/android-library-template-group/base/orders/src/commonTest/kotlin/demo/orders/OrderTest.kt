package demo.orders

import kotlin.test.Test
import kotlin.test.assertEquals

class OrderTest {
    @Test
    fun labelNamesTheId() {
        assertEquals("order 7", Order("7", 100).label())
    }
}
