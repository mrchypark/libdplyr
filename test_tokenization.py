import re

# Simulate simple tokenization
def simple_tokenize(input_str):
    tokens = []
    i = 0
    n = len(input_str)
    
    while i < n:
        # Skip whitespace
        while i < n and input_str[i] in ' \n\t':
            i += 1
        
        if i >= n:
            break
            
        ch = input_str[i]
        
        # Handle identifiers
        if ch.isalpha() or ch == '_':
            start = i
            while i < n and (input_str[i].isalnum() or input_str[i] in '_'):
                i += 1
            tokens.append(('IDENTIFIER', input_str[start:i]))
            continue
            
        # Handle operators
        if ch in ',()':
            tokens.append(('SYMBOL', ch))
            i += 1
            continue
            
        if ch == '=' and i + 1 < n and input_str[i+1] == '=':
            tokens.append(('SYMBOL', '=='))
            i += 2
            continue
            
        # Handle assignment
        if ch == '=':
            tokens.append(('ASSIGNMENT', ch))
            i += 1
            continue
            
        # Handle other symbols
        tokens.append(('UNKNOWN', ch))
        i += 1
    
    return tokens

# Test cases
test_cases = [
    'inner_join(df1, df2, by = "id")',
    'left_join(df1, df2, by = "id")',
    'select(name, age)',
    'filter(age > 18)',
]

for test in test_cases:
    print(f"\nInput: {test}")
    tokens = simple_tokenize(test)
    print("Tokens:")
    for token in tokens:
        print(f"  {token}")
    print()
