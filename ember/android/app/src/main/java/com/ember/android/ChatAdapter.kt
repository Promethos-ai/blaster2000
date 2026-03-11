package com.ember.android

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView

data class ChatMessage(
    val text: String,
    val isUser: Boolean
)

class ChatAdapter(
    private var messages: MutableList<ChatMessage> = mutableListOf()
) : RecyclerView.Adapter<RecyclerView.ViewHolder>() {

    companion object {
        private const val VIEW_USER = 0
        private const val VIEW_AI = 1
    }

    override fun getItemViewType(position: Int): Int =
        if (messages[position].isUser) VIEW_USER else VIEW_AI

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder {
        return if (viewType == VIEW_USER) {
            val v = LayoutInflater.from(parent.context).inflate(R.layout.item_message_user, parent, false)
            UserViewHolder(v)
        } else {
            val v = LayoutInflater.from(parent.context).inflate(R.layout.item_message_ai, parent, false)
            AiViewHolder(v)
        }
    }

    override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
        val msg = messages[position]
        when (holder) {
            is UserViewHolder -> holder.textView.text = msg.text
            is AiViewHolder -> holder.textView.text = msg.text
        }
    }

    override fun getItemCount(): Int = messages.size

    fun addUserMessage(text: String) {
        messages.add(ChatMessage(text, isUser = true))
        notifyItemInserted(messages.size - 1)
    }

    fun addAiMessage(text: String) {
        messages.add(ChatMessage(text, isUser = false))
        notifyItemInserted(messages.size - 1)
    }

    fun updateLastAiMessage(text: String) {
        val last = messages.lastIndex
        if (last >= 0 && !messages[last].isUser) {
            messages[last] = ChatMessage(text, isUser = false)
            notifyItemChanged(last)
        }
    }

    fun appendToLastAiMessage(append: String) {
        val last = messages.lastIndex
        if (last >= 0 && !messages[last].isUser) {
            val current = messages[last].text
            messages[last] = ChatMessage(current + append, isUser = false)
            notifyItemChanged(last)
        }
    }

    /** Append token to last AI message; if last is "Asking…", replace it with token. */
    fun appendTokenToLastAi(token: String, askingPlaceholder: String) {
        val last = messages.lastIndex
        if (last >= 0 && !messages[last].isUser) {
            val current = messages[last].text
            val newText = if (current == askingPlaceholder) token else current + token
            messages[last] = ChatMessage(newText, isUser = false)
            notifyItemChanged(last)
        }
    }

    fun ensureLastAiMessage() {
        val last = messages.lastIndex
        if (last < 0 || messages[last].isUser) {
            addAiMessage("")
        }
    }

    private class UserViewHolder(view: View) : RecyclerView.ViewHolder(view) {
        val textView: TextView = view.findViewById(R.id.message_text)
    }

    private class AiViewHolder(view: View) : RecyclerView.ViewHolder(view) {
        val textView: TextView = view.findViewById(R.id.message_text)
    }
}
