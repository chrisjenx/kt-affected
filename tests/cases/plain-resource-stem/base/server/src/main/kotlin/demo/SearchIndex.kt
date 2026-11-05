package demo

class SearchIndex {
    private val words = mutableSetOf<String>()

    fun add(word: String) {
        words += word
    }

    fun has(word: String): Boolean = word in words
}
