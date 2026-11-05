package demo.logic

import demo.Money
import kotlin.test.Test
import kotlin.test.assertEquals

class PriceLogicTest {
    @Test
    fun formats() = assertEquals("$2", Money(200).format())
}
