import pandas as pd
import os
import matplotlib.pyplot as plt

# Define column names
column_names = ['epoch_id', 'validator num', 'contributor num', 'batch num', 'tx size(Bytes)/tx', 'block num', 'epoch_time(latency)']

current_dir = os.getcwd()
parent_dir = os.path.dirname(current_dir)
# Read the markdown file
directory_path = parent_dir + '/test-data/test_data_7_30_1600_20_4_delay100/hbb-node1'
data_frames = []
for filename in os.listdir(directory_path):
    if filename.endswith(".md"):  # Only process .md files
        with open(os.path.join(directory_path, filename), 'r') as f:
            data = f.read()

        # Split the table into lines
        lines = data.split('\n')[3:-1]  # Exclude the header and the alignment line and the last empty line

        # Split each line into its components
        table_data = [line.split('|')[1:-1] for line in lines]  # Exclude the first and last empty strings

        # Convert the table data into a pandas DataFrame and append it to the list
        df = pd.DataFrame(table_data, columns=column_names)

        # Convert numeric columns to the correct data type
        for col in ['validator num', 'block num', 'epoch_time(latency)']:
            df[col] = pd.to_numeric(df[col])

        data_frames.append(df)

# Concatenate all data frames
df = pd.concat(data_frames)

grouped = df.groupby('validator num').agg({'block num': 'mean', 'epoch_time(latency)': 'mean'})

# Create a new figure
plt.figure(figsize=(20, 10))

# Create the first subplot for average throughput
plt.subplot(1, 2, 1)
plt.plot(grouped.index, grouped['block num'])
plt.xlabel('Number of Nodes')
plt.ylabel('Average Throughput')
plt.title('Average Throughput vs Number of Nodes')

# Create the second subplot for average latency
plt.subplot(1, 2, 2)
plt.plot(grouped.index, grouped['epoch_time(latency)'])
plt.xlabel('Number of Nodes')
plt.ylabel('Average Latency')
plt.title('Average Latency vs Number of Nodes')

# Create the directory to save figures
save_dir = os.path.join(parent_dir, 'analysis-fig')
os.makedirs(save_dir, exist_ok=True)

# Save the figures
save_path = os.path.join(save_dir, 'twopicture.png')
plt.savefig(save_path)

# Display the plots
# plt.show()
