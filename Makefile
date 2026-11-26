.PHONY: uji contoh native bukti banding bersih

uji:
	python3 -m unittest discover -s uji -p "uji_*.py" -v

contoh:
	./bin/nusa jalan contoh/halo.ns
	./bin/nusa jalan contoh/aman.ns
	./bin/nusa jalan contoh/fitur.ns
	./bin/nusa jalan contoh/kritis.ns

native:
	./bin/nusa bangun contoh/fitur.ns -o /tmp/fitur-native
	/tmp/fitur-native

banding:
	./bin/nusa jalan contoh/kritis.ns > /tmp/nusa-tafsir.txt
	./bin/nusa bangun contoh/kritis.ns -o /tmp/nusa-native >/dev/null
	/tmp/nusa-native > /tmp/nusa-native.txt
	diff /tmp/nusa-tafsir.txt /tmp/nusa-native.txt && echo "penafsir == kompiler"

bukti:
	./bin/nusa bukti contoh/kontrak.ns
	./bin/nusa bukti contoh/kritis.ns

bersih:
	rm -rf build dist *.egg-info
	find . -name "__pycache__" -type d -prune -exec rm -rf {} +
	find . -name "*.o" -delete
