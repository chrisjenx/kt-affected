package demo.authors

class AuthorIndex {
    private val names = mutableListOf<String>()

    fun add(name: String) {
        if (name.isNotBlank()) names += normalise(name)
    }

    fun count(): Int = names.size
}
