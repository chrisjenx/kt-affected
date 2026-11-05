package demo.orders

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceTest {
    @Test
    fun prefixesNumber() = assertEquals("INV-1", Invoice(1).label())
}
