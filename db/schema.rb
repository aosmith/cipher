# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.0].define(version: 2025_09_12_133207) do
  create_table "attachment_shares", force: :cascade do |t|
    t.integer "attachment_id", null: false
    t.integer "user_id", null: false
    t.text "encrypted_key", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["attachment_id", "user_id"], name: "index_attachment_shares_on_attachment_id_and_user_id", unique: true
    t.index ["attachment_id"], name: "index_attachment_shares_on_attachment_id"
    t.index ["user_id"], name: "index_attachment_shares_on_user_id"
  end

  create_table "attachments", force: :cascade do |t|
    t.integer "post_id", null: false
    t.string "filename"
    t.string "content_type"
    t.integer "file_size"
    t.text "data_encrypted"
    t.string "checksum"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.text "dev_owner_key"
    t.index ["post_id"], name: "index_attachments_on_post_id"
  end

  create_table "comments", force: :cascade do |t|
    t.integer "user_id", null: false
    t.integer "post_id", null: false
    t.text "content"
    t.datetime "timestamp"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["post_id"], name: "index_comments_on_post_id"
    t.index ["user_id"], name: "index_comments_on_user_id"
  end

  create_table "friendships", force: :cascade do |t|
    t.integer "requester_id", null: false
    t.integer "addressee_id", null: false
    t.string "status", default: "pending"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["addressee_id", "requester_id"], name: "index_friendships_on_addressee_id_and_requester_id"
    t.index ["addressee_id"], name: "index_friendships_on_addressee_id"
    t.index ["requester_id", "addressee_id"], name: "index_friendships_on_requester_id_and_addressee_id", unique: true
    t.index ["requester_id"], name: "index_friendships_on_requester_id"
    t.index ["status"], name: "index_friendships_on_status"
  end

  create_table "messages", force: :cascade do |t|
    t.integer "sender_id", null: false
    t.integer "recipient_id", null: false
    t.text "content"
    t.text "encrypted_content"
    t.datetime "read_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["created_at"], name: "index_messages_on_created_at"
    t.index ["recipient_id", "read_at"], name: "index_messages_on_recipient_id_and_read_at"
    t.index ["recipient_id"], name: "index_messages_on_recipient_id"
    t.index ["sender_id", "recipient_id", "created_at"], name: "index_messages_on_sender_id_and_recipient_id_and_created_at"
    t.index ["sender_id"], name: "index_messages_on_sender_id"
  end

  create_table "peers", force: :cascade do |t|
    t.integer "user_id", null: false
    t.string "address"
    t.integer "port"
    t.datetime "last_seen"
    t.text "public_key"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["user_id"], name: "index_peers_on_user_id"
  end

  create_table "posts", force: :cascade do |t|
    t.integer "user_id", null: false
    t.text "content_encrypted"
    t.text "signature"
    t.datetime "timestamp"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.integer "original_user_id"
    t.integer "synced_from_user_id"
    t.boolean "is_synced", default: false
    t.datetime "synced_at"
    t.string "content_hash"
    t.index ["content_hash"], name: "index_posts_on_content_hash"
    t.index ["is_synced"], name: "index_posts_on_is_synced"
    t.index ["original_user_id"], name: "index_posts_on_original_user_id"
    t.index ["synced_at"], name: "index_posts_on_synced_at"
    t.index ["synced_from_user_id"], name: "index_posts_on_synced_from_user_id"
    t.index ["user_id"], name: "index_posts_on_user_id"
  end

  create_table "sync_messages", force: :cascade do |t|
    t.integer "user_id", null: false
    t.integer "peer_id", null: false
    t.text "payload"
    t.string "message_type"
    t.string "status"
    t.integer "processed_count"
    t.integer "error_count"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["peer_id"], name: "index_sync_messages_on_peer_id"
    t.index ["user_id"], name: "index_sync_messages_on_user_id"
  end

  create_table "users", force: :cascade do |t|
    t.text "public_key"
    t.string "username"
    t.string "display_name"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.text "private_key"
  end

  add_foreign_key "attachment_shares", "attachments"
  add_foreign_key "attachment_shares", "users"
  add_foreign_key "attachments", "posts"
  add_foreign_key "comments", "posts"
  add_foreign_key "comments", "users"
  add_foreign_key "friendships", "users", column: "addressee_id"
  add_foreign_key "friendships", "users", column: "requester_id"
  add_foreign_key "messages", "users", column: "recipient_id"
  add_foreign_key "messages", "users", column: "sender_id"
  add_foreign_key "peers", "users"
  add_foreign_key "posts", "users"
  add_foreign_key "posts", "users", column: "original_user_id"
  add_foreign_key "posts", "users", column: "synced_from_user_id"
  add_foreign_key "sync_messages", "peers"
  add_foreign_key "sync_messages", "users"
end
