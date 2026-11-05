package demo

class Author(private val name: String) {
    fun initials(): String = name.take(1)
}
