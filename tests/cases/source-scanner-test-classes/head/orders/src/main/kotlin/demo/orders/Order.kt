package demo.orders

data class Order(val id: String, val total: Long, val currency: String = "EUR")
