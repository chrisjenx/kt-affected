package demo.feature

import demo.core.Money

class Invoice(private val total: Money) {
    fun line(): String = "Total: " + total.format()
}
