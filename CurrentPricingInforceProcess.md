
rollup_rate=.1
benefit_base_bonus=.3
premium_bonus=0
fixed_percentage=.25

demographics<-fread('C:/Users/KramerHowell/Desktop/FIA_inforce.csv') %>%
  filter(Benefit_Base>=10000,
         AttainedAge %>% between(55, 79)) %>%
  group_by(QualStatus=QualifiedStatus,
           IssueAge=round((AttainedAge-2)/5)*5 + 2,
           # IssueAge=AttainedAge,
           Gender,
           Benefit_Base_Bucket=case_when(
    Benefit_Base < 50000 ~ "[0, 50000)",
    Benefit_Base < 100000 ~ "[50000, 100000)",
    Benefit_Base < 200000 ~ "[100000, 200000)",
    Benefit_Base < 500000 ~ "[200000, 500000)",
    TRUE ~ "[500000, Inf)" 
  )) %>% 
  summarise(InitialPols=sum(n),
            InitialPremium=sum(Benefit_Base*n)/(1+benefit_base_bonus),
            InitialBB=sum(Benefit_Base*n)) %>% 
  ungroup()

activationEstimates<-fread('C:/Users/KramerHowell/Desktop/GLWB utilization rates_20241106.csv')

activationEstimates %>% 
  filter(RollupRate==.1,
          Rider_Rollup_Period==10,
          DistributionChannel=='IMO',
          Rider_Fee==0.01) %>% 
       filter(Duration %in% c(1:10, 12, 15, 20)) %>% 
       rename(QualStatus=QualifiedStatus) %>% 
  inner_join(demographics, by=join_by(IssueAge, QualStatus, Benefit_Base_Bucket)) %>% 
  bind_rows({.} %>% distinct(Gender, IssueAge, QualStatus, Benefit_Base_Bucket, InitialBB, InitialPols, InitialPremium) %>% 
              mutate(Duration=99, CumulativeUtilization=1)) %>% 
  arrange(Gender, IssueAge, QualStatus, Benefit_Base_Bucket, Duration) %>% 
  select(QualStatus, IssueAge, Gender, InitialBB, InitialPols, InitialPremium, Benefit_Base_Bucket,
         Duration, CumulativeUtilization) %>% 
  group_by(QualStatus, IssueAge, Gender, Benefit_Base_Bucket) %>% 
  mutate(Percentage=CumulativeUtilization - replace_na(lag(CumulativeUtilization), 0)) %>% 
  ungroup() %>% 
  cross_join(data.frame(CreditingStrategy=c('Fixed','Indexed'))) %>% 
  mutate(across(Percentage, ~if_else(CreditingStrategy=='Fixed', fixed_percentage, 1-fixed_percentage)*.x)) %>% 
  mutate(PolicyID=row_number(),
         PremiumSize=InitialBB/(1+benefit_base_bonus),
         SCPeriod=10,
         valRate=.0475,
         MGIR=.01,
         Bonus=premium_bonus,
         RollupType='Simple',
         Rollup=rollup_rate,
         RollupDuration=10,
         GLWBStartYear=Duration,
         WaitPeriod=GLWBStartYear - 1,
         across(Gender, ~if_else(.x == 'M','Male','Female'))
         ) %>% 
  mutate(across(c(InitialBB, InitialPols, InitialPremium), ~.x*100000000*Percentage/sum(InitialPremium*Percentage))) %>% 
  select(-Duration) 