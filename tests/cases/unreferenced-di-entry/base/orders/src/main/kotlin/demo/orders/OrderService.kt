package demo.orders

class OrderService(private val repo: OrderRepo) {
    fun exists(id: String): Boolean = repo.find(id) != null
}

fun service(repo: OrderRepo): OrderService = OrderService(repo)
