data %>% 
  select(name, age, salary) %>% 
  filter(age > 25) %>% 
  arrange(salary)