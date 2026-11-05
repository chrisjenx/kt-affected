package demo

class OrderService {
    fun place(item: String): String = "order:$item"
}

class InvoiceService {
    fun issue(amount: Int): String = "invoice:$amount"
}
