package demo.pricing

class IosFormat {
    fun format(cents: Int): String = "iOS " + Money(cents).format()
}
