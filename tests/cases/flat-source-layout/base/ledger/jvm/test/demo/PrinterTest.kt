package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class PrinterTest {
    @Test
    fun printsTheHeading() {
        assertEquals("INVOICE", Printer().header())
    }
}
