package demo.screens

import demo.Money
import kotlin.test.Test
import kotlin.test.assertEquals

class PriceScreenTest {
    @Test
    fun renders() = assertEquals("$1", Money(100).format())
}
