NUSA := ./target/release/nusa

.PHONY: semua uji contoh tafsir bersih

semua:
	cargo build --release

uji: semua
	./uji/jalankan.sh

contoh: semua
	@for f in contoh/*.ns; do $(NUSA) bangun "$$f" -o "/tmp/$$(basename $$f .ns)"; done

tafsir: semua
	$(NUSA) jalan contoh/pencarian.ns

bersih:
	cargo clean
