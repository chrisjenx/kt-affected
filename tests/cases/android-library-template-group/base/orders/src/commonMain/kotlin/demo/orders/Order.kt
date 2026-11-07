package demo.orders

data class Order(val id: String, val totalCents: Long) {
    fun label(): String = "order " + id
}
