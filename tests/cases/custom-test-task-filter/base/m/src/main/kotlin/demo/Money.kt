package demo

data class Money(val cents: Long) {
    fun format(): String = "$" + cents / 100
}
