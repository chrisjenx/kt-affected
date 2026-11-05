package demo

object Samples {
    fun all(): List<Any> = listOf(OrderPlaced(1))
    @Suppress("UNCHECKED_CAST")
    fun <T> first(): T = all().first() as T
}
