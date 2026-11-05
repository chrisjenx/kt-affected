package demo.pricing

class JsFormat {
    fun format(cents: Int): String = Money(cents).format()
}
