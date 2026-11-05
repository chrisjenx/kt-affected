package demo

class Checkout {
    fun pay(amount: Int): Int = amount.discounted()
}

class Receipt {
    fun print(amount: Int): String = "paid $amount"
}
