package demo.core

class Money(val cents: Int) {
    fun format(): String = "$" + cents / 100
}
