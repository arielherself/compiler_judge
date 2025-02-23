s = '-1'
for i in range(10 ** 5):
    s = f'-(({s}) + 1)'
print(f'public int main() {{\n  return ({s});\n}}')
