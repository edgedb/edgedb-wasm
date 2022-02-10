CREATE MIGRATION m13rb76beanbolinjy5mrws4ck5quuxwaxpo2cqdc356ovjcncygkq
    ONTO initial
{
  CREATE TYPE default::Counter {
      CREATE PROPERTY value -> std::int64;
  };
  INSERT Counter { value := 0 };
};
