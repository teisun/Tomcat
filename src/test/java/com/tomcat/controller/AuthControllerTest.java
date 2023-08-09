package com.tomcat.controller;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.tomcat.controller.request.AuthenticationRequest;
import org.junit.Before;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.test.web.servlet.ResultHandler;
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders;
import org.springframework.test.web.servlet.result.MockMvcResultMatchers;

import static org.junit.jupiter.api.Assertions.*;
@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
class AuthControllerTest {

    @Autowired
    private MockMvc mockMvc;


    private static Gson GSON;

    static {
        GSON = new Gson();
    }



    @Test
    void registerOrLogin() throws Exception{
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        String json = GSON.toJson(request);
        mockMvc.perform(MockMvcRequestBuilders
                .post("/registerOrLogin")
                .content(json.getBytes())
                .accept(MediaType.APPLICATION_JSON)
                .contentType(MediaType.APPLICATION_JSON_VALUE)
        ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }
}