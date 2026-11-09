package demo.billing

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceTest {
    @Test
    fun dumps() {
        assertEquals("1, 2", Invoice(listOf(1, 2)).debugDump())
    }
}
