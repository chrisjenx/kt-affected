package demo.orders

import demo.pricing.Money

class Order(val cents: Int) {
    fun total(): String = Money(cents).format()
}

class Invoice(val number: Int) {
    fun label(): String = "INV-$number"
}
