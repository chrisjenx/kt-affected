package demo.core

import kotlin.test.Test

class MoneySizeTest {
    @Test
    fun runs() {
        check(Money(5).cents == 5)
    }
}
