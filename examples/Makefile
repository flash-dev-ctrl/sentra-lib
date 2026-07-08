# sentra-lib C API example Makefile
DIST    := ../dist/apple-darwin-universal
TARGET  := cpp_example
CXX     := c++
CXXFLAGS := -std=c++17 -O2 -I$(DIST)/include
LDFLAGS  := -L$(DIST)/lib -lsentra -Wl,-rpath,$(abspath $(DIST)/lib)

CMD       ?= list
HOME_PATH ?= $(HOME)

.PHONY: all run clean

all: $(TARGET)

$(TARGET): cpp_example.cc
	$(CXX) $(CXXFLAGS) -o $@ $< $(LDFLAGS)

run: $(TARGET)
	./$(TARGET) $(CMD) $(HOME_PATH)

clean:
	rm -f $(TARGET)
