package demo.orders

class OrderRepository {
    private val orders = mutableMapOf<String, Order>()

    fun save(order: Order) {
        orders[order.id] = order
    }
}
