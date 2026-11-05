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

interface HorseRepo {
    fun horse(id: Int): String
}

class HorseRepoImpl(private val db: Db) : HorseRepo {
    override fun horse(id: Int): String = db.read(id)
}

interface Clock {
    fun now(): Long
}

class FixedClock : Clock {
    override fun now(): Long = 42L
}
