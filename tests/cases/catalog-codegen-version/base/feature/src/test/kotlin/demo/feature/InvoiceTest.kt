package demo.feature

import demo.core.Money
import kotlin.test.Test

class InvoiceTest {
    @Test
    fun runs() {
        Invoice(Money(100)).line()
    }
}
