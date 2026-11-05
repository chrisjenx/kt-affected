package demo.model

class Money(val cents: Int) {
    fun format(): String = cents.toString()
}
