package demo

data class Money(val cents: Long) {
    fun format(): String = "USD " + cents / 100
}
