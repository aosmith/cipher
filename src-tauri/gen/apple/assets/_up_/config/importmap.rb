# Pin npm packages by running ./bin/importmap

pin "application"
pin "@hotwired/turbo-rails", to: "turbo.min.js"
pin "@hotwired/stimulus", to: "stimulus.min.js"
pin "@hotwired/stimulus-loading", to: "stimulus-loading.js"
pin_all_from "app/javascript/controllers", under: "controllers"
pin "@rails/actioncable", to: "actioncable.esm.js"
pin_all_from "app/javascript/channels", under: "channels"

# Cryptography libraries - using skypack for ES6 modules
pin "tweetnacl", to: "https://cdn.skypack.dev/tweetnacl@1.0.3"
pin "tweetnacl-util", to: "https://cdn.skypack.dev/tweetnacl-util@0.15.1"
pin "crypto_utils", to: "crypto_utils.js"

# WebRTC and networking
pin "webrtc_manager", to: "webrtc_manager.js"

# Web3 and blockchain libraries
pin "ethers", to: "https://cdn.skypack.dev/ethers@5.7.2"
pin "web3_utils", to: "web3_utils.js"
pin "cipher_token", to: "cipher_token.js"
pin "blockchain_integration", to: "blockchain_integration.js"
pin "group_encryption", to: "group_encryption.js"
pin "friend_selector", to: "friend_selector.js"
pin "local_hosting", to: "local_hosting.js"
pin "p2p_hosting_integration", to: "p2p_hosting_integration.js"
pin "user_registration", to: "user_registration.js"
pin "post_creation", to: "post_creation.js"
pin "image_decryption", to: "image_decryption.js"
