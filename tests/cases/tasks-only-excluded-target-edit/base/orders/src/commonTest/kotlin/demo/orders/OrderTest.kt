package demo.orders

import kotlin.test.Test
import kotlin.test.assertEquals

class OrderTest {
    @Test
    fun sumsInDollars() = assertEquals("$1", Order(100).total())
}
