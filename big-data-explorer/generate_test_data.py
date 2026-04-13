import csv
import random

def generate_csv(filename, rows):
    headers = ["id", "timestamp", "category", "value", "status", "description"]
    categories = ["Financial", "Logistics", "Sales", "Support", "Inventory"]
    statuses = ["Paid", "Pending", "Cancelled", "Shipped", "Processing"]
    
    print(f"Generating {rows} rows into {filename}...")
    
    with open(filename, 'w', newline='') as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(headers)
        
        for i in range(1, rows + 1):
            row = [
                i,
                f"2026-02-{random.randint(1, 28):02d} {random.randint(0, 23):02d}:{random.randint(0, 59):02d}:{random.randint(0, 59):02d}",
                random.choice(categories),
                round(random.uniform(10.5, 9999.9), 2),
                random.choice(statuses),
                f"Generic transaction description for record {i}"
            ]
            writer.writerow(row)
            
            if i % 1000000 == 0:
                print(f"Reached {i} rows...")

if __name__ == "__main__":
    generate_csv("test_10m_rows.csv", 10000000)
    print("Done!")
