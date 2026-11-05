package demo.pricing

class Money(val cents: Int) {
    fun format(): String = "$" + cents / 100
}
