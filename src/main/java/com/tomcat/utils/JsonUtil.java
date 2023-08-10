package com.tomcat.utils;

import com.google.gson.Gson;
import org.springframework.stereotype.Component;

@Component
public class JsonUtil {


    Gson gson = new Gson();

    public String toJson(Object obj)  {
        return gson.toJson(obj);
    }

}
