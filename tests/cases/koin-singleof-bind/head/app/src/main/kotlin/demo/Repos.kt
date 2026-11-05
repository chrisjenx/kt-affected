package demo

class Db {
    fun read(id: Int): String = "row$id"
}

interface UserRepo {
    fun user(id: Int): String
    fun userCount(): Int
}

class UserRepoImpl(private val db: Db) : UserRepo {
    override fun user(id: Int): String = db.read(id)
    override fun userCount(): Int = 1
}

interface BookRepo {
    fun book(id: Int): String
}

class BookRepoImpl(private val db: Db) : BookRepo {
    override fun book(id: Int): String = db.read(id).uppercase()
}

interface Clock {
    fun now(): Long
}

class FixedClock : Clock {
    override fun now(): Long = 42L
}
