package demo.pricing

class IosFormat {
    fun format(cents: Int): String = Money(cents).format()
}
