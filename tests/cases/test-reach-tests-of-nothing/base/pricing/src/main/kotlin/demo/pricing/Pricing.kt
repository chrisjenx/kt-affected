package demo.pricing

class Pricing(private val rate: Long) {
    fun total(net: Long): Long = net + net * rate / 100
}
